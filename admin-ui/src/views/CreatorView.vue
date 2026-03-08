<script setup lang="ts">
import { ref, watch } from 'vue';
import { useFetch } from '@vueuse/core';
// import { useCounterStore } from '@/stores/counter';

interface StickerSetPub {
  id: string,
  title?: string,
}

const userId = ref("");

const { data, error, execute: refetch } = useFetch(() => `/api/unapproved-by-creator?userId=${userId.value}`, { refetch: true, updateDataOnError: true }).json<StickerSetPub[]>()

watch(data, () => console.log(data))

// const counter = useCounterStore();

const approve = async (setId: string) => {
    const { data, error } = await useFetch(`/api/sets/${setId}/approve`).post()
    console.log(data, error)
    if (error.value) {
      alert(error.value)
    }
  }

const unbanSet = async (setId: string) => {
    const { data, error } = await useFetch(`/api/sets/${setId}/unban`).post()
    if (error.value) {
      alert(error.value)
    }
}

const banSet = async (setId: string) => {
    const { data, error } = await useFetch(`/api/sets/${setId}/ban`).post()
    if (error.value) {
      alert(error.value)
    }
}

</script>

<template>
  <main>
    <div class="d-flex">
      <div>
        <v-text-field label="user id" v-model="userId" />
        <!-- <span @click="counter.increment()">
          {{ counter.count }}
        </span> -->
        <div v-if="data">
          <div v-for="set of data">
            <!-- <v-btn variant="plain" width="auto" height="auto" :to="{ name: 'banSetView', params: { setId: set.id } }"> -->
                <img :src="`/thumbnails/sticker-set/${set.id}/image.png`" loading="lazy" width="128" height="128" />
              <!-- </v-btn> -->
                
    <v-btn @click="approve(set.id)" color="success">
      Approve
    </v-btn>

    <v-btn @click="unbanSet(set.id)">
      Unban set
    </v-btn>
    <v-btn @click="banSet(set.id)" color="error">
      Ban set
    </v-btn>

                <span>
                  {{ set.id }}
                </span>
                
          </div>

        </div>
        <v-btn color="primary" @click="refetch()">
          refresh
        </v-btn>
        {{ error }}
      </div>
    </div>
  </main>
</template>
